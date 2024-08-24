
### Install Brain Brew package

https://pypi.org/project/Brain-Brew/

```shell
pipenv install brain-brew
```

### Run Local Version

Fork/Clone this repo onto your computer, then in a different repository you wish to run Brain Brew you can point it to this version in a 2 ways:

#### Install development folder for live updates

Point your installation to this folder. Run the following (change the path to match yours):

```shell
pipenv install -e ../brain-brew
```

This should result in your Pipfile updating to:

```
[packages]
brain-brew = {file = "../brain-brew", editable = true}
```

#### Install a locally built package

Build Brain Brew using the `scripts/build.bash` script. This will generate dist and build folders. Install the generated wheel by running:

```
pip install ../brain-brew/dist/Brain_Brew-0.3.11-py3-none-any.whl
```

This should result in your Pipfile updating to:

```
[packages]
brain-brew = {file = "../brain-brew/dist/Brain_Brew-0.3.11-py3-none-any.whl"}
```

Change to match the wheel version number, which is set in `brain_brew/front_matter.py` if you wish to change it.


