plugin.action("after_render", 10, function(ctx)
    local output = "public"
    local count = 0

    local function process_dir(dir)
        if not cblog.files.exists(dir) then return end
        local files = cblog.files.list(dir)
        if not files then return end
        for _, name in ipairs(files) do
            local path = dir .. "/" .. name
            if string.match(name, "%.html$") then
                local content = cblog.files.read(path)
                if content and string.find(content, "<img") then
                    local new_content = string.gsub(content, '<img([^>]-)(/?>)', function(attrs, close)
                        if string.find(attrs, 'loading=') then
                            return '<img' .. attrs .. close
                        end
                        count = count + 1
                        return '<img loading="lazy"' .. attrs .. close
                    end)
                    if new_content ~= content then
                        cblog.files.write(path, new_content)
                    end
                end
            end
        end
    end

    process_dir(output)
    if cblog.files.exists(output) then
        local entries = cblog.files.list(output)
        if entries then
            for _, name in ipairs(entries) do
                local subdir = output .. "/" .. name
                if not string.match(name, "%.") then
                    process_dir(subdir)
                end
            end
        end
    end

    if count > 0 then
        cblog.log.info("已为 " .. count .. " 个 img 标签添加 loading=lazy")
    end
end)
