import { Command, Flags } from '@oclif/core'
import {
  buildFieldsFromArgs,
  contentTypes,
  project,
  promptForString,
  selectAnOption,
  view,
} from '../utils'

import inflection from 'inflection'

import { ContentType, ContentTypeKindEnum } from '@kiqr/management-api-sdk'

export default class CreateCommand extends Command {
  static description = 'Create a new content type'
  static examples = [
    '<%= config.bin %> <%= command.id %>',
    '<%= config.bin %> <%= command.id %> --kind collection --name Posts --fields="title:string description:text"',
  ]

  static flags = {
    name: Flags.string({ char: 'n', description: 'Name of the content type' }),
    kind: Flags.enum({
      char: 'k',
      description: 'What kind of content type?',
      options: ['collection', 'component'],
    }),
    fields: Flags.string({ char: 'f', description: 'Initial fields' }),
    force: Flags.boolean(),
  }

  static args = [{ name: 'contentType', required: true }]

  public async run(): Promise<void> {
    if (!project.hasConfig()) {
      return view('errors/config-not-found')
    }

    const { args, flags } = await this.parse(CreateCommand)

    // Parse id from arguments
    const id = inflection.transform(args.contentType, ['dasherize'])

    // Ask user for a name of the content type.
    const nameQuestion = 'Give your content type a human readable name:'
    const nameSuggestion = inflection.transform(id, ['humanize', 'pluralize'])
    const name =
      flags.name ??
      inflection.transform(
        flags.name ?? (await promptForString(nameQuestion, nameSuggestion)),
        ['capitalize']
      )

    // Build fields object from parsed --fields flag
    const fields = flags.fields ? buildFieldsFromArgs(flags.fields) : []

    // Parse kind from fields or let the user choose from a list.
    const kind = flags.kind ?? (await this.promptForKind())

    // Build content type
    const contentType: ContentType = {
      id: id,
      name: name,
      kind: kind as ContentTypeKindEnum,
      associations: [],
      fields: fields,
    }

    // Store content type to file.
    contentTypes.saveToFile(contentType, flags.force)

    view('schemas/content-type-created', contentType)
  }

  private async promptForKind(): Promise<string> {
    const options = [
      [
        'collection',
        'Collection - Schema for a collection of resources (ex: blog posts, pages)',
      ],
      ['component', 'Component - Schema for a single component'],
    ]

    return selectAnOption(
      'What kind of component do you want to create?',
      options
    )
  }
}
