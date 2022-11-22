import { Project } from '@kiqr/management-api-sdk'
import { Command, Flags } from '@oclif/core'
import { dump } from 'js-yaml'
import {
  createDirectoryPath,
  createProject,
  findProjectFileInDirectory,
  promptForConfirmation,
  promptForString,
  ResponseError,
  session,
  view,
} from '../utils'

import inflection from 'inflection'
import fs from 'node:fs'
import path from 'node:path'

export default class InitCommand extends Command {
  targetDirectory: string | undefined = undefined

  static description = 'describe the command here'
  static examples = [
    '<%= config.bin %> <%= command.id %>',
    '<%= config.bin %> <%= command.id %> --name "My Project"',
  ]

  static flags = {
    name: Flags.string({ char: 'n', description: 'Name your new project' }),
  }

  static args = [{ name: 'targetDirectory' }]

  async run(): Promise<void> {
    const { isLoggedIn, token } = session()

    if (!isLoggedIn) {
      return view('errors/unauthenticated')
    }

    const { args, flags } = await this.parse(InitCommand)
    this.targetDirectory = path.resolve(args.targetDirectory ?? '.')

    // Check that theres no project in the targetDir already.
    await this.throwOrDeleteIfConflictingProject()

    // Create target directory if it does not exist.
    await createDirectoryPath(this.targetDirectory)

    // Set project name.
    const nameQuestion = 'What do you want to name your project?'
    const projectName = inflection.transform(
      flags.name ?? (await promptForString(nameQuestion)),
      ['capitalize']
    )

    // Build payload from user input.
    const payload = {
      name: projectName,
    }

    try {
      const project = await createProject(
        token?.access_token as string,
        payload
      )
      this.createKiqrFile(project)
    } catch (error: any) {
      if (error instanceof ResponseError) {
        view('errors/request', error)
      } else {
        console.log(error.message)
      }
    }
  }

  // Check for conflicting projects
  private async throwOrDeleteIfConflictingProject(): Promise<void> {
    if (!this.targetDirectory) return

    const conflictingFile = findProjectFileInDirectory(this.targetDirectory)

    if (conflictingFile) {
      this.log(`A kiqr.yaml file already exists at: ${conflictingFile}`)
      const deleteFile = await promptForConfirmation(
        'Choose "yes" if you want to delete it and replace it with a new project'
      )
      if (deleteFile) {
        fs.unlinkSync(conflictingFile)
      } else {
        throw new Error(`A kiqr.yaml file exists at: ${conflictingFile}`)
      }
    }
  }

  private createKiqrFile(project: Project): void {
    const filePath = this.targetDirectory + '/kiqr.yaml'
    const fileContents = { id: project.id, schemaVersion: 0 }

    fs.writeFileSync(filePath, dump(fileContents))
    view('init/success', { project: project, file: filePath })
  }
}
