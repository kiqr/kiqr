import Mustache from 'mustache'
import fs from 'node:fs'
import path from 'node:path'

export const view = (
  viewPath: string,
  options: Record<string, any> = {},
  trim = true
): void => {
  const template = fs.readFileSync(
    path.join(__dirname, `../views/${viewPath}.mustache`),
    'utf8'
  )
  const logo = fs.readFileSync(
    path.join(__dirname, '../views/globals/logo.mustache'),
    'utf8'
  )

  const logoView = Mustache.render(logo, {})

  let view = Mustache.render(template, options, {
    logo: logoView,
  })

  if (trim) {
    view = view.trim()
  }

  console.log(view)
}
