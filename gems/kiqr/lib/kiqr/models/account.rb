module Kiqr
  module Models
    module Account
      extend ActiveSupport::Concern
      include PublicUid::ModelConcern

      included do
        # Associations
        has_many :members, dependent: :destroy
        has_many :users, through: :members
        has_many :pending_invitations, -> { pending }, class_name: "Member"

        # Callbacks
        before_destroy :prevent_mistaken_deletion, prepend: true
        before_destroy :allow_owner_deletion, prepend: true

        # Validations
        validates :name, presence: true, length: { minimum: 3, maximum: 255 }
      end

      # Check if a user is a member of this account
      # @param user [User] The user to check
      # @return [Boolean] True if the user is a member, false otherwise
      def has_member?(user)
        users.include? user
      end

      # Check if this account is a team account
      # @return [Boolean] True if it's a team account, false if it's a personal account
      def team?
        !personal
      end

      private

      # Check if the account can be destroyed
      # @return [Boolean] True if the account has only one member, false otherwise
      def destroyable?
        members.size <= 1
      end

      # Prevent deletion of accounts with multiple members
      # @raise [Kiqr::Errors::AccountWithMembersDeletionError] If the account has more than one member
      def prevent_mistaken_deletion
        raise Kiqr::Errors::AccountWithMembersDeletionError unless destroyable?
      end

      # Allow deletion of the account owner
      # This method sets a flag on each member to skip the owner deletion prevention
      def allow_owner_deletion
        members.each { |member| member.skip_prevent_owner_deletion = true }
      end
    end
  end
end