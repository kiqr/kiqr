class User < ApplicationRecord
  include Kiqr::Users::Authentication
  include Kiqr::Users::Onboarding
  include Kiqr::Users::TwoFactorAuthentication
  include Kiqr::Users::Validations
  include Kiqr::Users::Omniauth
  include Kiqr::Users::Accounts

  # Personal account
  belongs_to :personal_account, class_name: "Account", optional: true, dependent: :destroy
  accepts_nested_attributes_for :personal_account
  validates_associated :personal_account

  # Get the user's full name from their personal account.
  delegate :name, to: :personal_account
end
