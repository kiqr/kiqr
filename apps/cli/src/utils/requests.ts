import {
  Configuration,
  CreateProjectRequest,
  CreateSchemaRequest,
  Project,
  ProjectsApi,
  Schema,
  SchemasApi,
  User,
  UserApi,
} from '@kiqr/management-api-sdk'

export interface ValidationErrorResponse {
  type: string
  message: string
  errors: string[]
}

export class ResponseError extends Error {
  public message: string
  public statusCode: number
  public type: string
  public errors: string[] | undefined

  constructor(
    message: string,
    statusCode: string,
    response: ValidationErrorResponse
  ) {
    super(message)

    // assign the status code
    this.statusCode = Number.parseInt(statusCode, 10)

    // assign the message
    this.message = response?.message ?? message

    // assign error messages
    this.type = response.type

    // assign error messages
    this.errors = response.errors

    // capturing the stack trace keeps the reference to your error class
    Error.captureStackTrace(this, this.constructor)
  }
}

const handleResponseError = (reject: any, originalError: any) => {
  if (originalError?.isAxiosError) {
    const responseError = new ResponseError(
      originalError.message, // error message
      originalError.response?.status, // status code
      originalError.response?.data // response body
    )
    reject(responseError)
  } else {
    reject(originalError) // pass original error if type is unknow.
  }
}

export const getUser = async (accessToken: string): Promise<User> => {
  const configuration = new Configuration({ accessToken: accessToken })
  const api = new UserApi(configuration)

  return new Promise((resolve, reject) => {
    api
      .getUser()
      .then((response) => resolve(response.data))
      .catch((error) => handleResponseError(reject, error))
  })
}

export const createProject = async (
  accessToken: string,
  payload: CreateProjectRequest
): Promise<Project> => {
  const configuration = new Configuration({ accessToken: accessToken })
  const api = new ProjectsApi(configuration)

  return new Promise((resolve, reject) => {
    api
      .createProject(payload)
      .then((response) => resolve(response.data))
      .catch((error) => handleResponseError(reject, error))
  })
}

export const createSchema = async (
  accessToken: string,
  projectId: string,
  lastSchemaVersion: string,
  payload: CreateSchemaRequest
): Promise<Schema> => {
  const configuration = new Configuration({ accessToken: accessToken })
  const api = new SchemasApi(configuration)

  return new Promise((resolve, reject) => {
    api
      .createSchema(projectId, lastSchemaVersion, payload)
      .then((response) => resolve(response.data))
      .catch((error) => handleResponseError(reject, error))
  })
}
