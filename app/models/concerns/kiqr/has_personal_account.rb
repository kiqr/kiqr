module Kiqr
  module HasPersonalAccount
    extend ActiveSupport::Concern

    included do
      # User belongs to a personal account.
      belongs_to :personal_account, class_name: "Account", optional: true, dependent: :destroy
      validates_associated :personal_account
      accepts_nested_attributes_for :personal_account

      # Get the user's full name from their personal account.
      delegate :name, to: :personal_account
    end

    def onboarded?
      personal_account.present? && personal_account.persisted?
    end
  end
end
