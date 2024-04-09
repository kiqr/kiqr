require_relative "lib/kiqr/version"

Gem::Specification.new do |spec|
  spec.name = "kiqr"
  spec.version = Kiqr::VERSION
  spec.authors = ["Rasmus Kjellberg"]
  spec.email = ["2277443+kjellberg@users.noreply.github.com"]
  spec.homepage = "https://kiqr.dev"
  spec.summary = "SaaS framework for Rails"
  spec.description = "KIQR is a complete solution to kickstart your SaaS development."

  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = "https://github.com/kiqr/kiqr"
  spec.metadata["changelog_uri"] = "https://github.com/kiqr/kiqr/blob/main/CHANGELOG.md"

  spec.files = Dir.chdir(File.expand_path(__dir__)) do
    Dir["{app,config,db,lib}/**/*", "MIT-LICENSE", "Rakefile", "README.md"]
  end

  spec.add_dependency "rails", ">= 7.1.3.2"
end
