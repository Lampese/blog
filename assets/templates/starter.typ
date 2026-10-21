#import "/typst/blog.typ": *
#post(id: "about-this-blog", title: "从这里开始", date: "2026-10-16", summary: "一篇可以直接修改的示例文章。", tags: ("笔记",))

这个站点由 Rust 构建，文章使用 Typst 编写。站点名称、作者和部署地址放在 `site.toml` 中。

= 写作

在 `.typ` 文件中保存文章，运行 `blog build` 生成网页，或用 `blog serve` 打开本地预览。

选中文字后，点击“复制引用”，把得到的 `#excerpt(...)` 代码粘贴到另一篇文章中，就可以嵌入这段持续更新的内容。

$ sum_(i=1)^n i = frac(n(n+1),2) $
