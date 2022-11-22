import type { ContentType } from '@kiqr/management-api-sdk'
import fs from 'node:fs'
import path from 'node:path'

/**
 * Search for a kiqr.yaml recursively in directores and parent directories.
 * @param {string} targetPath Path to target directory.
 * @returns {void}
 */
export const createDirectoryPath = async (
  targetPath: string
): Promise<void> => {
  // Extract filenames from targetPath
  targetPath = path.dirname(targetPath)

  // Create directory tree if it does not exist
  if (targetPath && !fs.existsSync(targetPath)) {
    fs.mkdirSync(targetPath, { recursive: true })
  }
}

/**
 * Search for a kiqr.yaml recursively in directores and parent directories.
 * @param {string | undefined} startDirectory Directory to start searching from.
 * @returns {string | undefined} path
 */
export const findProjectFileInDirectory = (
  startDirectory: string
): string | undefined => {
  const fileName = 'kiqr.yaml'
  let lastScanned = startDirectory

  if (fs.existsSync(path.join(startDirectory, fileName))) {
    return path.join(startDirectory, fileName)
  }

  const pathSegments = startDirectory.split('/').slice(1).reverse()

  for (let i = 0; i < pathSegments.length; i++) {
    const parentPath = path.resolve(lastScanned, '../')
    if (fs.existsSync(path.join(parentPath, fileName))) {
      return path.join(parentPath, fileName)
    }

    lastScanned = parentPath
  }
}

export const contentTypeDirectory = (): string => {
  const rootDir = findProjectRoot()
  if (!rootDir) throw new Error('Couldnt find kiqr file.')

  return path.join(rootDir, '/kiqr/content-types')
}

export const contentTypeFilePath = (contentType: ContentType) => {
  return path.join(contentTypeDirectory(), `${contentType.id}.yaml`)
}

export const findProjectRoot = (): string | undefined => {
  const projectFile = findProjectFileInDirectory(path.resolve())
  if (!projectFile) {
    return undefined
  }

  return path.dirname(projectFile)
}
