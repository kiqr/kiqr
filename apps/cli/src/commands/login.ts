import { Command } from '@oclif/core'
import { createSpinner } from 'nanospinner'
import {
  config,
  performAuthorization,
  selectAnOption,
  session,
  view,
} from '../utils'

import open from 'open'

export type LoginMethod = string

export default class Login extends Command {
  static description = 'Login using your KIQR ID.'
  static examples = ['<%= config.bin %> <%= command.id %>']

  public async run(): Promise<void> {
    view('login/welcome')

    const loginMethod = await this.determineLoginMethod()
    if (loginMethod === 'oauth') {
      this.loginUsingOauth()
    } else {
      this.log('Invalid login method selected.')
    }
  }

  private loginUsingOauth = async () => {
    const { authorizationUrl } = session()

    view('login/oauth', { url: authorizationUrl })
    open(authorizationUrl)
    const spinner = createSpinner(
      'Waiting for user to sign in via browser..'
    ).start()

    try {
      const accessToken = await performAuthorization()
      config.set('tokenType', 'oauth2')
      config.set('token', accessToken)
      spinner.success({ text: 'Successfully logged in!' })
      process.exit(0)
    } catch {
      spinner.error({ text: 'Invalid authenication code or denied by user.' })
    }
  }

  private async determineLoginMethod(): Promise<LoginMethod | undefined> {
    const options = [
      [
        'oauth',
        'KIQR ID - Login using your email or Github account (opens in web browser)',
      ],
    ]

    return selectAnOption('Select login method:', options)
  }
}
