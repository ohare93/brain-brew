import setuptools
from brain_brew.front_matter import latest_version_number

with open("README.md", "r") as fh:
    long_description = fh.read()

setuptools.setup(
    name="Brain-Brew",
    version=latest_version_number(),
    author="Jordan Munch O'Hare",
    author_email="brainbrew@jordan.munchohare.com",
    description="Automated Anki flashcard creation and extraction to/from Csv ",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/ohare93/brain-brew",
    packages=setuptools.find_packages(),
    include_package_data=True,
    entry_points={
        'console_scripts': [
            'brain_brew = brain_brew.main:main',
            'brain-brew = brain_brew.main:main',
            'brainbrew = brain_brew.main:main',
        ]
    },
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: Public Domain",
        "Operating System :: OS Independent",
    ],
    python_requires='>=3.7',
    install_requires=[
        'ruamel.yaml.clib>=0.2.2',
        'ruamel.yaml>=0.16.10',
        'yamale>=3.0.4'
    ]
)
