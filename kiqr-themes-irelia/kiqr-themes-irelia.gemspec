version = File.read(File.expand_path("../KIQR_VERSION", __dir__)).strip

Gem::Specification.new do |s|
  s.platform    = Gem::Platform::RUBY
  s.name        = "kiqr-themes-irelia"
  s.version     = version
  s.summary     = "Irelia views and assets for the KIQR framework."
  s.description = "Application views and assets for KIQR built with Irelia Components."

  s.required_ruby_version     = ">= 3.1.0"
  s.required_rubygems_version = ">= 1.8.11"

  s.license = "MIT"

  s.author   = "Rasmus Kjellberg"
  s.email    = "kjellberg@hey.com"
  s.homepage = "https://kiqr.dev"

  s.files        = Dir["MIT-LICENSE", "lib/**/*", "config/**/*"]
  s.require_path = "lib"

  s.metadata = {
    "bug_tracker_uri"   => "https://github.com/kiqr/kiqr/issues",
    "changelog_uri"     => "https://github.com/kiqr/kiqr/releases/tag/v#{version}",
    "documentation_uri" => "https://docs.kiqr.dev",
    "source_code_uri"   => "https://github.com/kiqr/kiqr/tree/v#{version}",
    "rubygems_mfa_required" => "true"
  }

  s.add_dependency "importmap-rails", ">= 2.0.1"
end
