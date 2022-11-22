/* eslint-disable camelcase */
import { config } from '.'

import axios from 'axios'
import express from 'express'

const BASE_URL = config.get<string>('authServerBaseUrl')
const REDIRECT_URL = 'http://localhost:28028'

export interface Oauth2Token {
  access_token: string
  token_type: 'Bearer'
  expires_in: number
  scope: string
  created_at: number
}

export const isValidToken = (token: Oauth2Token): boolean => {
  if (!token) return false

  const expiresAt = token.created_at + token.expires_in
  const secondsSinceEpoch = Math.round(Date.now() / 1000)
  const expiredToken = expiresAt < secondsSinceEpoch

  if (token && !expiredToken) {
    return true
  }

  return false
}

export const buildAuthorizationUrl = (): string => {
  const clientId = config.get<string>('clientId')
  const audience = config.get<string>('audience')
  const scope = 'profile+projects'
  const authorizationUrl = `${BASE_URL}/oauth/authorize?client_id=${clientId}&redirect_uri=${REDIRECT_URL}&response_type=code&scope=${scope}&audience=${audience}`

  return authorizationUrl
}

export const performAuthorization = async (): Promise<Oauth2Token> => {
  // Poll for the authorization code.
  const authorizationCode = await pollForAuthorizationCode()

  // Exchange the authorization code for an access token.
  const payload = {
    code: authorizationCode,
    client_id: config.get<string>('clientId'),
    audience: config.get<string>('audience'),
    redirect_uri: REDIRECT_URL,
    grant_type: 'authorization_code',
  }

  const response = await axios.post(`${BASE_URL}/oauth/token`, payload)
  return response.data
}

export const pollForAuthorizationCode = async (): Promise<string> => {
  return new Promise((resolve, reject) => {
    const app = express()
    let authStatus = 'failed'

    app.get('/', (req: express.Request, res: express.Response) => {
      if (req?.query?.code) {
        resolve(req.query.code.toString())
        authStatus = 'success'
      } else {
        reject(new Error('Invalid authentication code.'))
      }

      res.redirect(`${BASE_URL}/cli/auth/${authStatus}`)
      server.close()
    })

    const server = app.listen(28_028)
  })
}
