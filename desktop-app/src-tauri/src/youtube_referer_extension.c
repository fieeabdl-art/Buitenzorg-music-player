#include <string.h>
#include <webkit2/webkit-web-extension.h>
#include <libsoup/soup.h>

#define YOUTUBE_REFERER "https://com.buitenzorg.player"

static gboolean is_app_page(const gchar *uri) {
  if (uri == NULL) return FALSE;
  GUri *parsed = g_uri_parse(uri, G_URI_FLAGS_NONE, NULL);
  if (parsed == NULL) return FALSE;

  const gchar *scheme = g_uri_get_scheme(parsed);
  const gchar *host = g_uri_get_host(parsed);
  const gint port = g_uri_get_port(parsed);
  const gboolean production = g_strcmp0(scheme, "tauri") == 0 && g_strcmp0(host, "localhost") == 0;
  const gboolean development = g_strcmp0(scheme, "http") == 0
    && g_strcmp0(host, "127.0.0.1") == 0
    && port == 1420;
  const gboolean trusted = production || development;
  g_uri_unref(parsed);
  return trusted;
}

static gboolean is_youtube_player_request(const gchar *uri) {
  if (uri == NULL) return FALSE;
  GUri *parsed = g_uri_parse(uri, G_URI_FLAGS_NONE, NULL);
  if (parsed == NULL) return FALSE;

  const gchar *scheme = g_uri_get_scheme(parsed);
  const gchar *host = g_uri_get_host(parsed);
  const gchar *path = g_uri_get_path(parsed);
  const gboolean secure = g_strcmp0(scheme, "https") == 0;
  const gboolean youtube = g_strcmp0(host, "www.youtube.com") == 0;
  const gboolean player = g_strcmp0(path, "/iframe_api") == 0
    || (path != NULL && g_str_has_prefix(path, "/embed/"));
  const gboolean match = secure && youtube && player;
  g_uri_unref(parsed);
  return match;
}

static gboolean on_send_request(
  WebKitWebPage *page,
  WebKitURIRequest *request,
  WebKitURIResponse *redirected_response,
  gpointer user_data
) {
  (void) redirected_response;
  (void) user_data;
  if (!is_app_page(webkit_web_page_get_uri(page))) return FALSE;

  const gchar *uri = webkit_uri_request_get_uri(request);
  if (!is_youtube_player_request(uri)) return FALSE;

  SoupMessageHeaders *headers = webkit_uri_request_get_http_headers(request);
  if (headers != NULL) {
    soup_message_headers_replace(headers, "Referer", YOUTUBE_REFERER);
  }
  return FALSE;
}

static void on_page_created(WebKitWebExtension *extension, WebKitWebPage *page, gpointer user_data) {
  (void) extension;
  (void) user_data;
  g_signal_connect(page, "send-request", G_CALLBACK(on_send_request), NULL);
}

G_MODULE_EXPORT void webkit_web_extension_initialize(WebKitWebExtension *extension) {
  g_signal_connect(extension, "page-created", G_CALLBACK(on_page_created), NULL);
}