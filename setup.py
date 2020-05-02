import setuptools

with open("README.md", "r") as fh:
    long_description = fh.read()

setuptools.setup(
    name="Brain-Brew",
    version="0.1.3",
    author="Jordan Munch O'Hare",
    author_email="ohare93@gmail.com",
    description="Automated Anki flashcard creation and extraction to/from Csv ",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/ohare93/brain-brew",
    packages=setuptools.find_packages(),
    entry_points={
        'console_scripts': [
            'brain_brew = brain_brew.main:main',
            'brain-brew = brain_brew.main:main',
        ]
    },
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: Public Domain",
        "Operating System :: OS Independent",
    ],
    python_requires='>=3.6',
    install_requires=[
        'PyYAML'
    ]
)
