import Configstore from 'configstore'

export class Configuration {
  private config: Configstore

  constructor() {
    this.config = new Configstore('kiqr-cli', {
      audience: 'https://management-api.kiqr.cloud/',
      authServerBaseUrl: 'https://kiqr.eu.auth0.com',
      token: undefined,
      tokenType: 'oauth2',
      clientId: 'Dc3Y13bLmaWIBEdVPo4p49TD7SkUriUc',
      redirectUri: 'http://localhost:28028',
    })
  }

  /**
   * Get a config variable
   * @param {string} key The config key to receive the value from.
   * @returns {string} Current stored value.
   */
  get<T>(key: string): T {
    return this.config.get(key)
  }

  /**
   * Set a config variable.
   * @param {string} key The config key.
   * @param {string} value The config value.
   * @returns void
   */
  set<T>(key: string, value: T): void {
    this.config.set(key, value)
  }

  /**
   * Delete a config variable.
   * @param {string} key The config key.
   * @returns void
   */
  delete(key: string): void {
    this.config.delete(key)
  }
}

export const config = new Configuration()
