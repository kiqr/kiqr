import { Command } from '@oclif/core'
import { contentTypes, project, view } from '../utils'

export default class Inspect extends Command {
  static description = 'Display the contents of a content type file'
  static examples = ['<%= config.bin %> <%= command.id %>']

  static args = [{ name: 'contentType', required: true }]

  public async run(): Promise<void> {
    if (!project.hasConfig()) {
      return view('errors/config-not-found')
    }

    const { args } = await this.parse(Inspect)
    const contentTypeId: string = args.contentType
    const contentType = contentTypes.read(contentTypeId)

    if (!contentType) {
      this.log(`Invalid content type: ${contentTypeId}`)
      return
    }

    view('schemas/inspect-content-type', contentType)
  }
}
