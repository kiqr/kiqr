KIQR - CLI & SDKS
---------------------

#### What's included?

<table>
  <thead>
    <tr>
      <th>Source directory</th>
      <th>Version</th>
      <th>NPM package</th>
      <th>Description</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>apps/cli</td>
      <td><a href="https://www.npmjs.com/package/@kiqr%2Fcli" target="_blank"><img src="https://badge.fury.io/js/@kiqr%2Fcli.svg" /></a></td>
      <td>@kiqr/cli</td>
      <td>Command line application</td>
    </tr>
  </tbody>
</table>

Getting started
---------------

#### Setup workspace and install dependencies for all projects

```console
yarn install
```

#### Start local version of KIQR.CLOUD

```console
npx turbo dev
```

License
-------

The application is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).

Contributing
------------

New contributors are very welcome and needed. KIQR Headless CMS is an open-source, community project that anyone can contribute to. Reviewing and testing is highly valued and the most effective way you can contribute as a new contributor. It also will teach you much more about the code and process than opening pull requests.

Except for testing, there are several ways you can contribute to the betterment of the project:

- **Report an issue?** - If the issue isn’t reported, we can’t fix it. Please report any bugs, feature, and/or improvement requests on the [GitHub Issues tracker](https://github.com/kiqr/kiqr/issues).
- **Submit patches** - Do you have a new feature or a fix you'd like to share? [Submit a pull request](https://github.com/kiqr/kiqr/pulls)!
- **Write blog articles** - Are you using KIQR? We'd love to hear how you're using it in your projects. Write a tutorial and post it on your blog!

### Development process

The `main` branch is regularly built and tested, but it is not guaranteed to be completely stable. Tags are created regularly from release branches to indicate new official, stable release versions of the libraries.

#### Generate SDKs from OpenAPI specifications

```console
yarn generate:sdks 
```

### Commit message guidline

A good commit message should describe what changed and why. The KIQR-project uses [semantic commit messages](https://www.conventionalcommits.org/en/v1.0.0/) to streamline the release process.

Before a pull request can be merged, it must have a pull request title with a semantic prefix.

### Versioning

This application aims to adhere to [Semantic Versioning](http://semver.org/). Violations
of this scheme should be reported as bugs. Specifically, if a minor or patch
version is released that breaks backward compatibility, that version should be
immediately yanked and/or a new version should be immediately released that
restores compatibility. Breaking changes to the public API will only be
introduced with new major versions.
