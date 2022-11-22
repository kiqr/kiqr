import { config } from '.'
import { buildAuthorizationUrl, isValidToken, Oauth2Token } from './oauth'

interface SessionProps {
  authorizationUrl: string
  isLoggedIn: boolean
  token: Oauth2Token | undefined
}

export const session = (): SessionProps => {
  const token = config.get<Oauth2Token>('token')
  const isLoggedIn = isValidToken(token)
  const authorizationUrl = buildAuthorizationUrl()

  return { authorizationUrl, isLoggedIn, token }
}
