# Test Results

## Summary

| group | passed | % |
|-------|--------|---|
{% for group in groups %}
{{ group.name }} | {{ group.passed_info.num_passed }} / {{ group.passed_info.tot_tests }} | {{ group.passed_info.perc_passed }}
{% endfor %}

## Group Results

{% for group in groups %}
### {{ group.name }}
{% for sub_group in group.sub_groups %}
| sub-group | passed | % |
|-----------|--------|---|
{{ sub_group.name }} | (results/sub_groups/{{ sub_group.name }}) | {{ sub_group.passed_info.num_passed }} / {{ sub_group.passed_info.tot_tests }}  | {{ group.passed_info.perc_passed }} |

{% endfor %}
{% endfor %}
