# Test Results {{ filter_str_template }}

## Summary
| passed | % |
|--------|---|
| {{ passed_info.num_passed }} / {{ passed_info.tot_tests }} | {{ passed_info.perc_passed }} |

| name | status |
|------|--------|
{% for test in tests -%}
| {{ test.name }} | {{ test.status }} |
{% endfor %}
