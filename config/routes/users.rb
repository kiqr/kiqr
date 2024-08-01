devise_for :users, path_names: { sign_in: "login", sign_up: "create-account" }, controllers: {
  registrations: "users/registrations",
  sessions: "kiqr/sessions"
}

devise_scope :user do
  get "users/cancel-account", controller: "users/registrations", action: :cancel, as: :delete_user_registration
end

scope :users do
  # => Onboarding routes
  get "onboarding", controller: "users/onboarding", action: :new
  patch "onboarding", controller: "users/onboarding", action: :update
end

namespace :user, path: nil, module: :users do
  namespace :settings do
    resource "profile", only: [ :show, :update ]
  end
end