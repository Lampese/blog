#import "/typst/blog.typ": *
#site-page(title: "东灯 / Lampese")

我是东灯，也叫 Lampese。目前供职于 MoonBit 和 Lody，也是创业公司 Calculus Singularity 的创始人。

#html.elem("p", attrs: (class: "profile-elsewhere"))[
  #link("https://github.com/Lampese")[GitHub ↗]
]

= 经历

// Earlier appointments follow https://github.com/Lampese/Lampese.
// The two appointments starting in September 2026 were supplied by the author.
#experience(
  "Loro / Lody", url: "https://loro.dev",
  logos: ("/images/organizations/loro.svg", "/images/organizations/lody.png"),
  description: [
    #link("https://loro.dev")[Loro] 是面向本地优先应用的 CRDT 库#linebreak()
    #link("https://lody.ai")[Lody] 是供团队共享和使用编程智能体的工作空间
  ],
)[
  #experience-position("Developer", from: "2026-09", team: "Lody")
  #experience-position("Algorithm Intern", from: "2024-07", until: "2024-08", team: "Loro")
]

#experience(
  "MoonBit", url: "https://www.moonbitlang.com", logos: ("/images/organizations/moonbit.png",),
  description: [通用编程语言及其工具链，包含编译器、运行时、IDE 和构建系统。],
)[
  #experience-position("Programming Language Tool Developer", from: "2026-09")
  #experience-position("AI Intern", from: "2025-12", until: "2026-02")
  #experience-position("Programming Language Tool Development Intern", from: "2025-01", until: "2025-03")
]

#experience(
  "Calculus Singularity", url: "https://calculus-singularity.com", logos: ("/images/organizations/calculus-singularity.png",),
  description: [关注开源、AI、函数式编程与编程语言理论的初创团队，开发 SaaS 系统和语言工具。],
)[
  #experience-position("Founder / Team Leader", from: "2026-02")
]

#experience(
  "ISCAS PLCT Lab", url: "https://github.com/plctlab", logos: ("/images/organizations/iscas.png",),
  description: [中科院软件所的工具链团队，主要从事 RISC-V 编译器、模拟器和语言虚拟机的开发。],
)[
  #experience-position("Internship Student Mentor", from: "2024-12", until: "2026-01")
]

#experience(
  "Rustica", url: "https://github.com/rustica-lang", logos: ("/images/organizations/rustica.png",),
  description: [面向数据库应用的编程语言与运行时，支持语言内查询，并通过 PostgreSQL 运行 WebAssembly。],
)[
  #experience-position("Compiler Intern", from: "2024-09", until: "2025-12")
]

= 教育经历

#education(
  "温州肯恩大学", url: "https://www.wku.edu.cn", logo: "/images/organizations/wku.png",
  from: "2024-09", until: "2029-06", note: [（预计，2026 年休学）],
)

#education(
  "东营市第一中学", logo: "/images/organizations/dyyz.svg",
  from: "2021-09", until: "2024-06", note: [（曾获 NOIP 一等奖）],
)
