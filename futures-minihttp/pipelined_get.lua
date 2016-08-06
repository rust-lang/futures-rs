arguments = {}

init = function(args)
    arguments = args
end

request = function()
    pipeline_length = tonumber(arguments[1]) or 1
    local r = {}

    for i=1,pipeline_length do
        r[i] = wrk.format("GET", "/plaintext")
    end

    req = table.concat(r)

    return req
end
