-module(caller).
-export([run/0, run_with/1]).

run() ->
    handler:limit_for(#{limit => 5}).

run_with(Env) ->
    Opts = #{limit => list_to_integer(os:getenv(Env))},
    handler:limit_for(Opts).
