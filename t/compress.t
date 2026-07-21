# Test::Nginx::Socket suite for ngx_http_compress_module. Runs against a nginx
# binary with the module compiled in (TEST_NGINX_BINARY). See docker/test-nginx.sh.
use Test::Nginx::Socket 'no_plan';

run_tests();

__DATA__

=== TEST 1: gzip negotiated -> Content-Encoding + Vary
--- config
location = /t {
    compress on;
    compress_gzip on;
    compress_min_length 1;
    return 200 "hello hello hello hello hello hello hello hello\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: gzip
--- response_headers
Content-Encoding: gzip
Vary: Accept-Encoding
--- error_code: 200

=== TEST 2: no Accept-Encoding -> identity, body untouched
--- config
location = /t {
    compress on;
    compress_gzip on;
    compress_min_length 1;
    return 200 "plain body plain body plain body\n";
}
--- request
GET /t
--- response_headers
! Content-Encoding
--- response_body
plain body plain body plain body
--- error_code: 200

=== TEST 3: below min_length -> not compressed
--- config
location = /t {
    compress on;
    compress_gzip on;
    compress_min_length 100000;
    return 200 "tiny\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: gzip
--- response_headers
! Content-Encoding
--- error_code: 200

=== TEST 4: client quality picks brotli over gzip
--- config
location = /t {
    compress on;
    compress_gzip on;
    compress_brotli on;
    compress_min_length 1;
    return 200 "negotiate negotiate negotiate negotiate\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: gzip;q=0.5, br;q=1.0
--- response_headers
Content-Encoding: br
--- error_code: 200

=== TEST 5: `compress balanced` is turnkey (enables codecs, sets preset min_length)
--- config
location = /t {
    compress balanced;
    return 200 "compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: gzip
--- response_headers
Content-Encoding: gzip
Vary: Accept-Encoding
--- error_code: 200

=== TEST 6: explicit `compress_zstd off` overrides the `max` profile
--- config
location = /t {
    compress max;
    compress_zstd off;
    return 200 "compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload compressible payload\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: zstd
--- response_headers
! Content-Encoding
--- error_code: 200

=== TEST 7: preset min_length applies -> short body under the balanced threshold stays identity
--- config
location = /t {
    compress balanced;
    return 200 "short body\n";
}
--- request
GET /t
--- more_headers
Accept-Encoding: gzip
--- response_headers
! Content-Encoding
--- error_code: 200
