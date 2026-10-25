//! Runs tasks when they are due, and publishes scheduled assets. One loop
//! for the whole installation, ticking every 30 seconds; a run that was due
//! while the daemon was down runs once if it is under an hour late,
//! otherwise it is recorded as missed. A scheduled publish is never missed:
//! it goes out on the first tick after its time.

use crate::{agent, App};
use chrono::Utc;
use kiqr_core::events::EventKind;
use kiqr_core::project::Project;
use kiqr_core::tasks::{self, Run, RunStatus, Task, TaskStatus, CATCH_UP};
use std::time::Duration;

pub const TICK: Duration = Duration::from_secs(30);

/// Close what a previous daemon left open: runs still `running`.
pub fn recover(app: &App) {
    let Ok(projects) = Project::list(&app.store) else { return };
    for p in projects {
        for mut t in Task::list(&app.store, &p.slug).unwrap_or_default() {
            let mut changed = false;
            for mut r in t.runs(&app.store, &p.slug).unwrap_or_default().into_iter().filter(|r| r.status == RunStatus::Running) {
                r.status = RunStatus::Failed;
                r.finished = Some(Utc::now());
                if r.summary.is_empty() {
                    r.summary = "The daemon stopped while this run was in progress.".into();
                }
                let _ = r.save(&app.store, &p.slug);
                changed = true;
            }
            if changed {
                t.last_status = Some(RunStatus::Failed);
                let _ = t.save(&app.store, &p.slug);
            }
        }
    }
}

pub async fn run(app: App) {
    let mut first = true;
    loop {
        tick(&app, first).await;
        first = false;
        tokio::time::sleep(TICK).await;
    }
}

async fn tick(app: &App, startup: bool) {
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    let now = Utc::now();
    let Ok(projects) = Project::list(&app.store) else { return };
    for p in projects {
        crate::publish::publish_due(app, &p.slug, now).await;
        for mut t in Task::list(&app.store, &p.slug).unwrap_or_default() {
            if t.status != TaskStatus::Active {
                continue;
            }
            let Some(due) = t.next_run else { continue };
            if due > now {
                continue;
            }
            if app.jobs.read().await.contains(&format!("task:{}", t.id)) {
                continue;
            }
            let late = now - due;
            if late > CATCH_UP {
                // Too long ago: note it and move on to the next occurrence.
                let r = Run { id: tasks::run_id(due), task: t.id.clone(), started: due, finished: Some(now), status: RunStatus::Missed, trigger: "schedule".into(), summary: format!("Due {} but the daemon was not running.", due.with_timezone(&tz).format("%a %-d %b %H:%M")), steps: vec![], assets: vec![], session_id: None, cost_usd: None };
                let _ = r.save(&app.store, &p.slug);
                t.last_run = Some(due);
                t.last_status = Some(RunStatus::Missed);
                t.runs += 1;
                if t.schedule.kind == tasks::ScheduleKind::Once {
                    t.status = TaskStatus::Done;
                    t.next_run = None;
                } else if let Err(e) = t.plan_next(now, tz) {
                    tracing::warn!(slug = %p.slug, task = %t.id, error = %e, "could not plan the next run");
                    t.status = TaskStatus::Paused;
                }
                let _ = t.save(&app.store, &p.slug);
                let _ = app.store.commit(&format!("[auto] task({}): missed {}", p.slug, t.title)).await;
                app.bus.emit(Some(&p.slug), EventKind::TaskRunFinished { task: t.id.clone(), run: r.id, status: "missed".into(), summary: r.summary });
                agent::emit_task(app, &p.slug, &t);
                continue;
            }
            // Plan the next one before running, so a slow run can't double-fire.
            if t.schedule.kind == tasks::ScheduleKind::Rrule {
                if let Err(e) = t.plan_next(now, tz) {
                    tracing::warn!(slug = %p.slug, task = %t.id, error = %e, "could not plan the next run");
                    t.status = TaskStatus::Paused;
                    t.next_run = None;
                }
            } else {
                t.next_run = None;
            }
            if let Err(e) = t.save(&app.store, &p.slug) {
                tracing::warn!(slug = %p.slug, task = %t.id, error = %e, "could not save the task");
                continue;
            }
            let trigger = if startup { "startup" } else { "schedule" };
            if let Err(e) = agent::start_run(app, &p.slug, &t.id, trigger).await {
                tracing::warn!(slug = %p.slug, task = %t.id, error = %e, "could not start the run");
            }
        }
    }
}
