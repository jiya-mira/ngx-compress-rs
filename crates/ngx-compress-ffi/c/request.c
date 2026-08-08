#include <ngx_config.h>
#include <ngx_core.h>
#include <ngx_http.h>

/*
 * Keep request bitfield writes in C.  ngx_http_request_t starts its bitfield
 * storage after an in_port_t, and some ABIs insert padding between successive
 * unsigned storage units.  Bindgen models the fields as one packed byte array,
 * which can address the wrong bit on those targets (notably aarch64).
 */
void
ngx_compress_prepare_encoded_response(ngx_http_request_t *r)
{
    r->main_filter_need_in_memory = 1;
    ngx_http_clear_content_length(r);
    ngx_http_clear_accept_ranges(r);
    ngx_http_weak_etag(r);
}
