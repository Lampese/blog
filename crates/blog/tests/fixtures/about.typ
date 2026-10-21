#import "/typst/blog.typ": *
#let heading = "关于" + "此处"
#site-page(title: heading, summary: "一份可独立编辑的介绍。")

#include "intro.typ"

= 写作

中文、English 与 $a^2 + b^2 = c^2$。

#text(fill: rgb("#345678"))[为这一页单独设置的样式。]

#link("/articles/")[浏览文章]

#playground("console.log(1 + 2);")

#experience("Example Organization", url: "https://example.org", logos: ("/marks/organization.svg",))[
  #experience-position("Tooling Intern", from: "2026-09")
  #experience-position("Algorithm Intern", from: "2024-07", until: "2024-08", team: "Research")
]
