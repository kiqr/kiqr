# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[7.2].define(version: 2024_04_24_103325) do
  create_table "accounts", force: :cascade do |t|
    t.string "public_uid"
    t.string "name", null: false
    t.boolean "personal", default: false, null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["public_uid"], name: "index_accounts_on_public_uid", unique: true
  end

  create_table "members", force: :cascade do |t|
    t.string "public_uid"
    t.integer "user_id"
    t.integer "account_id", null: false
    t.integer "invited_by_id"
    t.boolean "owner", default: false, null: false
    t.string "invitation_email"
    t.string "invitation_token"
    t.datetime "invitation_sent_at"
    t.datetime "invitation_accepted_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["account_id", "invitation_email"], name: "index_members_on_account_id_and_invitation_email", unique: true
    t.index ["account_id", "user_id"], name: "index_members_on_account_id_and_user_id", unique: true
    t.index ["account_id"], name: "index_members_on_account_id"
    t.index ["invitation_token"], name: "index_members_on_invitation_token", unique: true
    t.index ["invited_by_id"], name: "index_members_on_invited_by_id"
    t.index ["public_uid"], name: "index_members_on_public_uid", unique: true
    t.index ["user_id"], name: "index_members_on_user_id"
  end

  create_table "omniauth_identities", force: :cascade do |t|
    t.string "public_uid"
    t.integer "user_id", null: false
    t.string "provider", null: false
    t.string "provider_uid", null: false
    t.text "credentials"
    t.text "info"
    t.text "extra"
    t.datetime "expires_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["public_uid"], name: "index_omniauth_identities_on_public_uid", unique: true
    t.index ["user_id"], name: "index_omniauth_identities_on_user_id"
  end

  create_table "users", force: :cascade do |t|
    t.string "email", default: "", null: false
    t.string "encrypted_password", default: "", null: false
    t.string "reset_password_token"
    t.datetime "reset_password_sent_at"
    t.datetime "remember_created_at"
    t.integer "sign_in_count", default: 0, null: false
    t.datetime "current_sign_in_at"
    t.datetime "last_sign_in_at"
    t.string "current_sign_in_ip"
    t.string "last_sign_in_ip"
    t.string "confirmation_token"
    t.datetime "confirmed_at"
    t.datetime "confirmation_sent_at"
    t.string "unconfirmed_email"
    t.integer "failed_attempts", default: 0, null: false
    t.string "unlock_token"
    t.datetime "locked_at"
    t.string "otp_secret"
    t.integer "consumed_timestep"
    t.boolean "otp_required_for_login", default: false
    t.text "otp_backup_codes"
    t.string "locale", default: "en"
    t.string "time_zone", default: "UTC"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.integer "personal_account_id"
    t.index ["confirmation_token"], name: "index_users_on_confirmation_token", unique: true
    t.index ["email"], name: "index_users_on_email", unique: true
    t.index ["personal_account_id"], name: "index_users_on_personal_account_id"
    t.index ["reset_password_token"], name: "index_users_on_reset_password_token", unique: true
    t.index ["unlock_token"], name: "index_users_on_unlock_token", unique: true
  end

  add_foreign_key "members", "accounts"
  add_foreign_key "members", "members", column: "invited_by_id"
  add_foreign_key "members", "users"
  add_foreign_key "omniauth_identities", "users"
  add_foreign_key "users", "accounts", column: "personal_account_id"
end
