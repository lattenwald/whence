-module(p).
-export([go/0]).

-record(req, {body}).

read_body(#req{body = B}) -> B.

go() -> read_body(#req{body = hello}).
