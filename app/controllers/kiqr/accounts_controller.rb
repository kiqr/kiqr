module Kiqr
  class AccountsController < ApplicationController
    before_action :ensure_team_and_setup_account, only: %i[edit update]
    before_action :setup_breadcrumbs, only: %i[edit update]

    def new
      @account = Account.new
    end

    def create
      @account = Account.new(account_params)

      if @account.valid?
        Kiqr::Services::Teams::Create.call!(account: @account, user: current_user)
        kiqr_flash_message(:notice, :account_created)
        redirect_to after_select_account_path(account_id: @account)
      else
        render :new, status: :unprocessable_entity
      end
    end

    def update
      @account.assign_attributes(account_params)

      if @account.valid?
        Kiqr::Services::Teams::Update.call!(account: @account, user: current_user)
        kiqr_flash_message(:notice, :account_updated)
        redirect_to edit_account_path
      else
        render :edit, status: :unprocessable_entity
      end
    end

    private

    def account_params
      params.require(:account).permit(Kiqr.config.account_attributes)
    end

    def ensure_team_and_setup_account
      @account = Account.find(current_account.id)
      redirect_to edit_settings_path if @account.personal?
    end

    def form_submit_path
      @account.persisted? ? account_path : accounts_path
    end
    helper_method :form_submit_path

    def form_method
      @account.persisted? ? :patch : :post
    end
    helper_method :form_method

    def setup_breadcrumbs
      add_breadcrumb "Team settings", edit_account_path(current_account)
    end
  end
end
