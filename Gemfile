source "https://rubygems.org"
git_source(:github) { |repo| "https://github.com/#{repo}.git" }

RAILS_VERSION = "~> 7.1"

# Specify your gem's dependencies in kiqr.gemspec.
gemspec

gem "puma"
gem "rails", RAILS_VERSION
gem "sqlite3", "~> 2.0"
gem "sprockets-rails"

# Start debugger with binding.b [https://github.com/ruby/debug]
# gem "debug", ">= 1.0.0"

group :development, :test do
  # See https://guides.rubyonrails.org/debugging_rails_applications.html#debugging-with-the-debug-gem
  # gem "brakeman", "~> 6.1"
  # gem "debug", platforms: %i[mri windows]

  # Omakase Ruby styling [https://github.com/rails/rubocop-rails-omakase/]
  gem "rubocop-rails-omakase", require: false

  # gem "standard", require: false
  # gem "erb_lint", require: false
  # gem "letter_opener_web", "~> 2.0"
  gem "factory_bot", "~> 6.4.3"
  # gem "rails-controller-testing"
end

group :test do
  # Use system testing [https://guides.rubyonrails.org/testing.html#system-testing]
  gem "capybara"
  gem "selenium-webdriver"
  gem "simplecov", require: false
  gem "faker"
  gem "rails-controller-testing"
end
