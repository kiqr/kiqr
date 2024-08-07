module Kiqr
  module FlashMessages
    extend ActiveSupport::Concern

    included do
      # Add more types of flash message to match the available variants of Irelia::Notification::Component.
      add_flash_types :success, :warning
    end

    # Render the flash messages stream
    def render_flash_messages_stream
      turbo_stream.replace("flash_messages", partial: "partials/flash_message")
    end

    private

    # Set a flash message with a translation key
    def kiqr_flash_message(type, message, **kwargs)
      flash[type] = I18n.t("flash_messages.#{message}", **kwargs)
    end

    def kiqr_flash_message_now(type, message, **kwargs)
      flash.now[type] = I18n.t("flash_messages.#{message}", **kwargs)
    end
  end
end
