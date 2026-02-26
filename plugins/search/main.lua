plugin.action("after_finalize", 10, function(ctx)
    local index = {}
    if ctx.posts then
        for i, post in ipairs(ctx.posts) do
            local content = ""
            if post.content then
                content = string.sub(cblog.strip_html(post.content), 1, 500)
            end
            table.insert(index, {
                id = post.id or tostring(i),
                title = post.title or "",
                url = post.url or "",
                content = content,
                tags = post.tags or {},
                date = post.created_at or "",
            })
        end
    end
    local output = ctx.output_dir or "public"
    cblog.files.write(output .. "/search-index.json", cblog.json(index))
    cblog.log.info("已生成搜索索引：" .. #index .. " 篇文章")
end)
