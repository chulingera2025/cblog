plugin.action("after_render", 20, function(ctx)
    local output = ctx.output_dir or "public"
    local css = [[
<style>
html { scroll-behavior: smooth; }
.toc-list { list-style: none; padding-left: 0; }
.toc-list li { margin: 4px 0; }
.toc-list a { color: #4a6cf7; text-decoration: none; }
.toc-list a:hover { text-decoration: underline; }
</style>
]]

    if cblog.files.exists(output) then
        local files = cblog.files.list(output)
        if files then
            for _, name in ipairs(files) do
                if string.match(name, "%.html$") then
                    local path = output .. "/" .. name
                    local content = cblog.files.read(path)
                    if content and string.find(content, "toc%-list") then
                        content = string.gsub(content, "</head>", css .. "</head>")
                        cblog.files.write(path, content)
                    end
                end
            end
        end
    end

    cblog.log.debug("toc: 平滑滚动样式已注入")
end)
