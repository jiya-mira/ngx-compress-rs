#include <ngx_config.h>
#include <ngx_core.h>
#include <ngx_http.h>

/*
 * Keep NGINX bitfield access and the ngx_post_event macro in C.  Bindgen's
 * synthetic bitfield layout is not a portable ABI contract, notably on arm64.
 */
void
ngx_compress_set_buffered(ngx_http_request_t *r, ngx_uint_t pending)
{
    if (pending) {
        r->connection->buffered |= NGX_HTTP_GZIP_BUFFERED;
    } else {
        r->connection->buffered &= ~NGX_HTTP_GZIP_BUFFERED;
    }
}

void
ngx_compress_post_write_if_ready(ngx_http_request_t *r)
{
    if (r->connection->buffered == NGX_HTTP_GZIP_BUFFERED) {
        ngx_post_event(r->connection->write, &ngx_posted_next_events);
    }
}
