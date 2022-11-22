import { Command } from '@oclif/core'
import { getUser, session, view } from '../utils'

export default class MeCommand extends Command {
  static description = "Shows who's logged in."
  static examples = ['<%= config.bin %> <%= command.id %>']

  public async run(): Promise<void> {
    const { isLoggedIn, token } = session()

    if (!isLoggedIn) {
      return view('errors/unauthenticated', {}, true)
    }

    const user = await getUser(token?.access_token as string)
    view('user/me', { user: user })
  }
}
