CLI for KIQR Headless CMS
======================================

[![npm version](https://badge.fury.io/js/@kiqr%2Fcli.svg)](https://badge.fury.io/js/@kiqr%2Fcli)

This is the Command Line Application for [KIQR Headless CMS](https://kiqr.dev).

Installation
------------

Run the following command using npm:

```console
npm install @kiqr/management-api-sdk --save
```

This will download and add `@kiqr/management-api-sdk` to your projects `package.json`-file.

Commands
--------

<!-- commands -->
* [`kiqr create CONTENTTYPE`](#kiqr-create-contenttype)
* [`kiqr hello PERSON`](#kiqr-hello-person)
* [`kiqr hello world`](#kiqr-hello-world)
* [`kiqr help [COMMAND]`](#kiqr-help-command)
* [`kiqr init [TARGETDIRECTORY]`](#kiqr-init-targetdirectory)
* [`kiqr inspect CONTENTTYPE`](#kiqr-inspect-contenttype)
* [`kiqr login`](#kiqr-login)
* [`kiqr logout`](#kiqr-logout)
* [`kiqr me`](#kiqr-me)
* [`kiqr plugins`](#kiqr-plugins)
* [`kiqr plugins:install PLUGIN...`](#kiqr-pluginsinstall-plugin)
* [`kiqr plugins:inspect PLUGIN...`](#kiqr-pluginsinspect-plugin)
* [`kiqr plugins:install PLUGIN...`](#kiqr-pluginsinstall-plugin-1)
* [`kiqr plugins:link PLUGIN`](#kiqr-pluginslink-plugin)
* [`kiqr plugins:uninstall PLUGIN...`](#kiqr-pluginsuninstall-plugin)
* [`kiqr plugins:uninstall PLUGIN...`](#kiqr-pluginsuninstall-plugin-1)
* [`kiqr plugins:uninstall PLUGIN...`](#kiqr-pluginsuninstall-plugin-2)
* [`kiqr plugins update`](#kiqr-plugins-update)
* [`kiqr push`](#kiqr-push)

## `kiqr create CONTENTTYPE`

Create a new content type

```
USAGE
  $ kiqr create [CONTENTTYPE] [-n <value>] [-k collection|component] [-f <value>] [--force]

FLAGS
  -f, --fields=<value>               Initial fields
  -k, --kind=(collection|component)  What kind of content type?
  -n, --name=<value>                 Name of the content type
  --force

DESCRIPTION
  Create a new content type

EXAMPLES
  $ kiqr create

  $ kiqr create --kind collection --name Posts --fields="title:string description:text"
```

_See code: [dist/commands/create.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/create.ts)_

## `kiqr hello PERSON`

Say hello

```
USAGE
  $ kiqr hello [PERSON] -f <value>

ARGUMENTS
  PERSON  Person to say hello to

FLAGS
  -f, --from=<value>  (required) Whom is saying hello

DESCRIPTION
  Say hello

EXAMPLES
  $ oex hello friend --from oclif
  hello friend from oclif! (./src/commands/hello/index.ts)
```

_See code: [dist/commands/hello/index.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/hello/index.ts)_

## `kiqr hello world`

Say hello world

```
USAGE
  $ kiqr hello world

DESCRIPTION
  Say hello world

EXAMPLES
  $ kiqr hello world
  hello world! (./src/commands/hello/world.ts)
```

## `kiqr help [COMMAND]`

Display help for kiqr.

```
USAGE
  $ kiqr help [COMMAND] [-n]

ARGUMENTS
  COMMAND  Command to show help for.

FLAGS
  -n, --nested-commands  Include all nested commands in the output.

DESCRIPTION
  Display help for kiqr.
```

_See code: [@oclif/plugin-help](https://github.com/oclif/plugin-help/blob/v5.1.12/src/commands/help.ts)_

## `kiqr init [TARGETDIRECTORY]`

describe the command here

```
USAGE
  $ kiqr init [TARGETDIRECTORY] [-n <value>]

FLAGS
  -n, --name=<value>  Name your new project

DESCRIPTION
  describe the command here

EXAMPLES
  $ kiqr init

  $ kiqr init --name "My Project"
```

_See code: [dist/commands/init.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/init.ts)_

## `kiqr inspect CONTENTTYPE`

Display the contents of a content type file

```
USAGE
  $ kiqr inspect [CONTENTTYPE]

DESCRIPTION
  Display the contents of a content type file

EXAMPLES
  $ kiqr inspect
```

_See code: [dist/commands/inspect.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/inspect.ts)_

## `kiqr login`

Login using your KIQR ID.

```
USAGE
  $ kiqr login

DESCRIPTION
  Login using your KIQR ID.

EXAMPLES
  $ kiqr login
```

_See code: [dist/commands/login.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/login.ts)_

## `kiqr logout`

Login using your KIQR ID.

```
USAGE
  $ kiqr logout

DESCRIPTION
  Login using your KIQR ID.

EXAMPLES
  $ kiqr logout
```

_See code: [dist/commands/logout.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/logout.ts)_

## `kiqr me`

Shows who's logged in.

```
USAGE
  $ kiqr me

DESCRIPTION
  Shows who's logged in.

EXAMPLES
  $ kiqr me
```

_See code: [dist/commands/me.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/me.ts)_

## `kiqr plugins`

List installed plugins.

```
USAGE
  $ kiqr plugins [--core]

FLAGS
  --core  Show core plugins.

DESCRIPTION
  List installed plugins.

EXAMPLES
  $ kiqr plugins
```

_See code: [@oclif/plugin-plugins](https://github.com/oclif/plugin-plugins/blob/v2.1.0/src/commands/plugins/index.ts)_

## `kiqr plugins:install PLUGIN...`

Installs a plugin into the CLI.

```
USAGE
  $ kiqr plugins:install PLUGIN...

ARGUMENTS
  PLUGIN  Plugin to install.

FLAGS
  -f, --force    Run yarn install with force flag.
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Installs a plugin into the CLI.

  Can be installed from npm or a git url.

  Installation of a user-installed plugin will override a core plugin.

  e.g. If you have a core plugin that has a 'hello' command, installing a user-installed plugin with a 'hello' command
  will override the core plugin implementation. This is useful if a user needs to update core plugin functionality in
  the CLI without the need to patch and update the whole CLI.

ALIASES
  $ kiqr plugins add

EXAMPLES
  $ kiqr plugins:install myplugin 

  $ kiqr plugins:install https://github.com/someuser/someplugin

  $ kiqr plugins:install someuser/someplugin
```

## `kiqr plugins:inspect PLUGIN...`

Displays installation properties of a plugin.

```
USAGE
  $ kiqr plugins:inspect PLUGIN...

ARGUMENTS
  PLUGIN  [default: .] Plugin to inspect.

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Displays installation properties of a plugin.

EXAMPLES
  $ kiqr plugins:inspect myplugin
```

## `kiqr plugins:install PLUGIN...`

Installs a plugin into the CLI.

```
USAGE
  $ kiqr plugins:install PLUGIN...

ARGUMENTS
  PLUGIN  Plugin to install.

FLAGS
  -f, --force    Run yarn install with force flag.
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Installs a plugin into the CLI.

  Can be installed from npm or a git url.

  Installation of a user-installed plugin will override a core plugin.

  e.g. If you have a core plugin that has a 'hello' command, installing a user-installed plugin with a 'hello' command
  will override the core plugin implementation. This is useful if a user needs to update core plugin functionality in
  the CLI without the need to patch and update the whole CLI.

ALIASES
  $ kiqr plugins add

EXAMPLES
  $ kiqr plugins:install myplugin 

  $ kiqr plugins:install https://github.com/someuser/someplugin

  $ kiqr plugins:install someuser/someplugin
```

## `kiqr plugins:link PLUGIN`

Links a plugin into the CLI for development.

```
USAGE
  $ kiqr plugins:link PLUGIN

ARGUMENTS
  PATH  [default: .] path to plugin

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Links a plugin into the CLI for development.

  Installation of a linked plugin will override a user-installed or core plugin.

  e.g. If you have a user-installed or core plugin that has a 'hello' command, installing a linked plugin with a 'hello'
  command will override the user-installed or core plugin implementation. This is useful for development work.

EXAMPLES
  $ kiqr plugins:link myplugin
```

## `kiqr plugins:uninstall PLUGIN...`

Removes a plugin from the CLI.

```
USAGE
  $ kiqr plugins:uninstall PLUGIN...

ARGUMENTS
  PLUGIN  plugin to uninstall

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Removes a plugin from the CLI.

ALIASES
  $ kiqr plugins unlink
  $ kiqr plugins remove
```

## `kiqr plugins:uninstall PLUGIN...`

Removes a plugin from the CLI.

```
USAGE
  $ kiqr plugins:uninstall PLUGIN...

ARGUMENTS
  PLUGIN  plugin to uninstall

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Removes a plugin from the CLI.

ALIASES
  $ kiqr plugins unlink
  $ kiqr plugins remove
```

## `kiqr plugins:uninstall PLUGIN...`

Removes a plugin from the CLI.

```
USAGE
  $ kiqr plugins:uninstall PLUGIN...

ARGUMENTS
  PLUGIN  plugin to uninstall

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Removes a plugin from the CLI.

ALIASES
  $ kiqr plugins unlink
  $ kiqr plugins remove
```

## `kiqr plugins update`

Update installed plugins.

```
USAGE
  $ kiqr plugins update [-h] [-v]

FLAGS
  -h, --help     Show CLI help.
  -v, --verbose

DESCRIPTION
  Update installed plugins.
```

## `kiqr push`

Push your local changes to KIQR.CLOUD

```
USAGE
  $ kiqr push [-m <value>]

FLAGS
  -m, --message=<value>  Write a meaningful commit message

DESCRIPTION
  Push your local changes to KIQR.CLOUD

EXAMPLES
  $ kiqr push

  $ kiqr push --force
```

_See code: [dist/commands/push.ts](https://github.com/kiqr/cli/blob/v0.0.18/dist/commands/push.ts)_
<!-- commandsstop -->
