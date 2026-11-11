# BUG (safety): the result of parsing is thrown away, so a failure goes unnoticed
def parse_length(text: str) -> tuple[bool, float | str]:
    try:
        return True, float(text)
    except ValueError:
        return False, "not a number: " + text


parse_length("12,5")
print("parsed")
