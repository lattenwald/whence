-module(d).
-export([apply_cb/2]).

apply_cb(Cb, X) ->
    R = Cb(X),
    R.
