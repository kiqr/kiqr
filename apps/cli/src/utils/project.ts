import { dump, load } from 'js-yaml'
import { findProjectFileInDirectory, findProjectRoot } from './'

import fs from 'node:fs'
import path from 'node:path'

export interface ProjectConfig {
  id: string
  schemaVersion: number
  [key: string]: any
}

const hasConfig = (): boolean => {
  const configPath = findProjectRoot()
  return Boolean(configPath)
}

const parseProjectConfigFromFile = (): ProjectConfig | undefined => {
  const configPath = findProjectFileInDirectory(path.resolve())
  if (!configPath) {
    return undefined
  }

  return load(fs.readFileSync(configPath, 'utf8')) as ProjectConfig
}

const getConfigValue = <T>(key: keyof ProjectConfig): T | undefined => {
  const projectConfig = parseProjectConfigFromFile()
  if (projectConfig && projectConfig[key]) {
    return projectConfig[key] as T
  }
}

const setConfigValue = <T>(key: string, value: T): void => {
  const projectConfig = parseProjectConfigFromFile()
  const filePath = findProjectFileInDirectory(path.resolve())

  if (projectConfig && filePath) {
    projectConfig[key] = value
    fs.writeFileSync(filePath, dump(projectConfig))
  }
}

export const project = {
  hasConfig,
  getConfigValue,
  setConfigValue,
}
