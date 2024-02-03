# jrsx

![CI](https://github.com/cvng/jrsx/actions/workflows/ci.yml/badge.svg?branch=main)

A clean `JSX` syntax for your [Askama][1] templates.

```html
<!-- Before -->
{%- import "hello.html" as hello_scope -%}
{%- import "child.html" as child_scope -%}

{% call hello_scope::hello(name) %}
{% call hello_scope::hello(name=name) %}
{% call hello_scope::hello(name="world") %}
{% call child_scope::child() %}Super!{% endcall %}
```

```html
<!-- After -->
<Hello name />
<Hello name=name />
<Hello name="world" />
<Child>Super!</Child>
```

[1]: https://djc.github.io/askama
