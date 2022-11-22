/* eslint-disable camelcase */
import { Command, Flags } from '@oclif/core'
import {
  contentTypes,
  createSchema,
  project,
  promptForString,
  ResponseError,
  session,
  view,
} from '../utils'

export default class PushCommand extends Command {
  static description = 'Push your local changes to KIQR.CLOUD'
  static examples = [
    '<%= config.bin %> <%= command.id %>',
    '<%= config.bin %> <%= command.id %> --force',
  ]

  static flags = {
    message: Flags.string({
      char: 'm',
      description: 'Write a meaningful commit message',
    }),
  }

  public async run(): Promise<void> {
    const { isLoggedIn, token } = session()

    if (!isLoggedIn) {
      return view('errors/unauthenticated')
    }

    if (!project.hasConfig()) {
      return view('errors/config-not-found')
    }

    const { flags } = await this.parse(PushCommand)
    const payload = {
      message:
        flags.message ??
        (await promptForString(
          'Write a meaningful push message that other developers can understand:'
        )),
      data: {
        content_types: contentTypes.all(),
      },
    }

    try {
      const projectId = project.getConfigValue<string>('id')
      if (!projectId) throw new Error('Missing project ID in kiqr.yaml')

      const schemaVersion = (
        project.getConfigValue<number>('schemaVersion') ?? 0
      ).toString()
      const schema = await createSchema(
        token?.access_token as string,
        projectId,
        schemaVersion,
        payload
      )
      project.setConfigValue<number>('schemaVersion', schema.version)

      view('schemas/created', schema)
    } catch (error: any) {
      if (error instanceof ResponseError) {
        view('errors/request', error)
      } else {
        console.log(error)
      }
    }
  }
}
