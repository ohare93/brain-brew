# See credentials in ~/.pypirc

# To use an API token:
#
# Set your username to __token__
# Set your password to the token value, including the pypi- prefix

# Upload
twine upload dist/* --verbose
