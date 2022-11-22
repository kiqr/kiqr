import { Command } from '@oclif/core'
import { config } from '../utils'

export default class LogoutCommand extends Command {
  static description = 'Login using your KIQR ID.'
  static examples = ['<%= config.bin %> <%= command.id %>']

  public async run(): Promise<void> {
    config.delete('token')
    config.delete('tokenType')
    this.log('Successfully logged out!')
  }
}
