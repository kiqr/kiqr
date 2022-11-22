import type {
  ContentType,
  ContentTypeField,
  ContentTypeFieldTypeEnum,
} from '@kiqr/management-api-sdk'
import { dump, load } from 'js-yaml'
import {
  contentTypeDirectory,
  contentTypeFilePath,
  createDirectoryPath,
} from './'

import inflection from 'inflection'
import _ from 'lodash'
import fs from 'node:fs'
import path from 'node:path'

export const buildFieldsFromArgs = (fieldsFlag: string): ContentTypeField[] => {
  const explodedFieldsFlag = fieldsFlag.split(' ')
  const fields: ContentTypeField[] = []

  for (const field of explodedFieldsFlag) {
    const segments = field.split(':')
    const [name, type] = segments
    fields.push({
      id: name,
      label: inflection.titleize(name),
      type: (type ?? 'string') as ContentTypeFieldTypeEnum,
      required: false,
    })
  }

  return fields
}

const saveContentTypeToFile = (contentType: ContentType, force = false) => {
  const targetPath = contentTypeFilePath(contentType)

  // Create content types directory if it doesn't exists.
  createDirectoryPath(targetPath)

  // Check if file exists already.
  if (fs.existsSync(targetPath) && !force) {
    throw new Error('A kiqr.json file exists already at: ' + targetPath)
  }

  fs.writeFileSync(targetPath, dump(contentType), {
    encoding: 'utf8',
    flag: 'w',
  })
}

const readContentTypeFromFile = (id: string): ContentType | undefined => {
  let fileName = id

  // Add .yaml extension if its missing.
  if (!_.endsWith(fileName, '.yaml')) {
    fileName += '.yaml'
  }

  // Full filePath to content type file.
  const filePath = path.join(contentTypeDirectory(), fileName)

  // Return a ContentType if it exists already.
  if (fs.existsSync(filePath)) {
    const data = load(fs.readFileSync(filePath, 'utf8')) as ContentType
    return data
  }
}

const getAllContentTypes = (): any => {
  // Create content types directory if it doesn't exists.
  const targetDir = contentTypeDirectory()
  createDirectoryPath(contentTypeDirectory())

  const contentTypeFiles = []
  for (const filename of fs.readdirSync(targetDir)) {
    if (_.endsWith(filename, '.yaml')) {
      contentTypeFiles.push(readContentTypeFromFile(filename))
    }
  }

  return contentTypeFiles
}

export const contentTypes = {
  saveToFile: saveContentTypeToFile,
  all: getAllContentTypes,
  read: readContentTypeFromFile,
}
