# Test Results {{ filter_str_template }}

## Summary
| passed | % |
|--------|---|
| {{ passed_info.num_passed }} / {{ passed_info.tot_tests }} | passed_info.perc_passed |


{% for test in tests %}
| name | status |
| {{ test.name }} | test.status |
{% endfor %}
