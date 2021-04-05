# Brain-Brew

<a title="Buy me a cuppa tea" href="https://ko-fi.com/brainbrew"><img src="https://img.shields.io/badge/ko--fi-contribute-%23579ebd.svg"></a>
<a title="Support a fellow Weekend Warrior on Patreon" href="https://www.patreon.com/jmohare?fan_landing=true"><img src="https://img.shields.io/badge/patreon-support-%23f96854.svg"></a>

Brain Brew is an open-source flashcard manipulation tool designed to allow users to convert their Anki flashcards to/from many different formats to suit their own needs.
The goal is to facilitate collaboration and maximize user choice, with a powerful tool that minimizes effort.
[CrowdAnki](https://github.com/Stvad/CrowdAnki) Exports and Csv(s) are the only supported file types as of now, but there will be more to come.


[Anki Ultimate Geography](https://github.com/axelboc/anki-ultimate-geography/) is currently the best working example of a Flashcard repo using Brain Brew :tada:
See there for inspiration!


# Installation


Install the latest version of [Brain Brew on PyPi.org](https://pypi.org/project/Brain-Brew/)
with `pip install brain-brew`. Virtual environment using `pipenv` is recommended!

:exclamation: See the [Brain Brew Starter Project][BrainBrewStarter]for a working clone-able Git repo.
From this repo you can now create a functional Brain Brew setup automatically, 
with your own flashcards! Simply by running

```bash
brainbrew init [Your CrowdAnki Export Folder]
```

This will generate the entire working repo for you, including the recipe files, source files, and build folder.
For bi-directional sync: Anki <-> Source!

See [the starter repo][BrainBrewStarter] for a step-by-step guide for all of this.

# Usage

Brain Brew runs from the command line and takes a *Recipe.yaml* file to run.

```bash
brainbrew run source_to_anki.yaml
```

Full usage help text:
```bash
Brain Brew vx.y.z
usage: brainbrew [-h] {run,init} ...

Manage Flashcards by transforming them to various types.

positional arguments:
  {run,init}  Commands that can be run
    run       Run a recipe file. This will convert some data to another format, based on the instructions in the recipe file.
    init      Initialise a Brain Brew repository, using a CrowdAnki export as the base data.

optional arguments:
  -h, --help  show this help message and exit
```


## Recipes

These are the instructions for how Brain Brew will ~~build~~ *brew* your data into another format.

What's YAML? See the current spec [here](http://www.yaml.org/spec/1.2/spec.html).

Run a recipe with `--verify` or `-v` to confirm your recipe is valid, without actually running it.
A dry run of sorts.

### Tasks

A recipe is made of many individual tasks, which do specific functions. 
Full detailed list coming soon™️, but see the [Yamale recipe schema](https://github.com/ohare93/brain-brew/blob/master/brain_brew/schemas/recipe.yaml) 
(local file: `brain_brew/schemas/recipe.yaml`) in the meantime :+1:




[//]: <> (Yamale)

# The Why

Brain Brew was made in an effort to solve some of the following issues with current collaboration of Anki Flashcards:

#### Sharing Personal Information or Copyrighted Material

Have some personal notes on your cards? Used some images randomly taken from the internet? 
That usually means you cannot share your deck entirely, without having to go to the effort of removing the offending material and/or managing two separate copies.

#### Having to Pick Between Source Control or Anki Editing

Putting your cards into a source control system brings a lot of benefits. 
You can see any changes that occur, go back in time should an mistake be discovered, and collaborate with others.

However the current tools for managing Anki cards in source control 
(such as [Anki-DM](https://github.com/OnkelTem/anki-dm), [GenAnki](https://github.com/kerrickstaley/genanki), 
and [Remote Decks](https://github.com/c-okelly/anki-remote-decks)) are only one way.
You generate cards from a csv into a file that can *only be imported* into Anki. 
There is no way to export them back, meaning a user must manually copy their changes over, or simple not edit their cards anywhere other than in source control.

This robs the user of two important work flows:
1. Editing/fixing cards in Anki as you review them (on desktop or mobile)
1. The plethora of Anki add-ons that already exist that are amazingly useful. E.g: Image Occlusion, Morphman, AwesomeTTS. 

A user should not have to pick between these fantastic work flows and the usage of source control to structure, manage, and share their cards.

#### Lack of Formatting Choice

Csvs are great for editing data, but can only go so far by themselves. Having all the data inside one csv leaves a lot to be desired and can result in eventual problems.
When one gets as many columns as *this* (from [Ultimate Geography](https://github.com/axelboc/anki-ultimate-geography/)) then it becomes a nightmare to manage:

|guid|Country|Country:de|Country:es|Country:fr|Country:nb|"Country info"|"Country info:de"|"Country info:es"|"Country info:fr"|"Country info:nb"|Capital|Capital:de|Capital:es|Capital:fr|Capital:nb|"Capital info"|"Capital info:de"|"Capital info:es"|"Capital info:fr"|"Capital info:nb"|"Capital hint"|"Capital hint:de"|"Capital hint:es"|"Capital hint:fr"|"Capital hint:nb"|Flag|"Flag similarity"|"Flag similarity:de"|"Flag similarity:es"|"Flag similarity:fr"|"Flag similarity:nb"|Map|tags|
| ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- |
|crr.AfnVRi|England|England|Inglaterra|Angleterre|England|"Constituent country of the United Kingdom."|"Landesteil des Vereinigten Königreichs."|"Nación constitutiva del Reino Unido."|"Nation constitutive du Royaume-Uni."|"Land som utgjør en del av Storbritannia."|London|London|Londres|Londres|London| | | | | |"Not a sovereign country"|"Kein souveräner Staat"|"No es un país soberano"|"Pas une nation souveraine"|"Ikke selvstendig land"|"<img src=""ug-flag-england.svg"" />"| | | | | |"<img src=""ug-map-england.png"" />"|UG::Europe|
"h<B?Kff,?3"|"Ivory Coast"|Elfenbeinküste|"Costa de Marfil"|"Côte d’Ivoire"|Elfenbenskysten|"Officially Côte d'Ivoire."|"Offiziell Côte d'Ivoire."|"Oficialmente Côte d'Ivoire."| | |Yamoussoukro|Yamoussoukro|Yamusukro|Yamoussoukro|Yamoussoukro|"While Yamoussoukro is the official capital, Abidjan is the de facto seat of government."|"Yamoussoukro ist die offizielle Hauptstadt, aber Abidjan ist der Regierungssitz."|"Aunque Yamusukro es la capital oficial, Abiyán es la capital de facto."|"Bien que Yamoussoukro soit la capitale officielle, Abidjan est le siège du gouvernement."|"Yamoussoukro er offisiell hovedstad, mens Abidjan er de facto regjeringssete."| | | | | |"<img src=""ug-flag-ivory_coast.svg"" />"|"Ireland (orange and green flipped, wider)"|"Irland (Orange und Grün vertauscht, breiter)"|"Irlanda (naranja y verde intercambiados, más ancha)"|"Irlande (orange et vert inversés, plus large)"|"Ireland (byttet plass på oransje og grønt, bredere)"|"<img src=""ug-map-ivory_coast.png"" />"|"UG::Africa UG::Sovereign_State UG::West_Africa"

Then there's having too many rows in one csv for it to be properly managed.


# Features of Brain Brew
### Multi-directional Card Syncing
Make changes in your source file and sync those into your Anki collection.

Make changes inside Anki and pull those back into the source.

Any user of your shared deck can make a change inside Anki and at some later point export their deck (or just part of it) using CrowdAnki.
Then the source file can be updated with their changes and a new CrowdAnki Export for all users to import can be generated with one run of Brain Brew.

### Modular Configuration Files 
Yaml config files are what drive the conversion of Brain Brew, allowing users to easily change the functionality as they wish.

<!--
Reusable subconfig files allow for minor changes without breaking the DRY principle.
-->

```Yaml
- generate_guids_in_csv:
    source: src/data/words.csv
    columns: [ guid ]

- build_parts:
  - note_model_from_yaml_part:
      part_id: LL Word
      file: src/note_models/LL Word.yaml

  - headers_from_yaml_part:
      part_id: default header
      file: src/headers/default.yaml
      override:  # Optional
        deck_description_html_file: src/headers/desc.html

  - media_group_from_folder:
      part_id: all_media
      source: src/media
      recursive: true  # Optional

  - notes_from_csvs:
      part_id: english-to-danish

      note_model_mappings:
        - note_models:
            - LL Word
          columns_to_fields:  # Optional
            guid: guid
            tags: tags

            english: English
            danish: Word
            picture: Picture
            danish audio: Pronunciation (Recording and/or IPA)
      
      file_mappings:
        - file: src/data/words.csv
          note_model: LL Word
          sort_by_columns: [english]  # Optional
          reverse_sort: no  # Optional
```

### Personal Fields 
Deck managers can set specific fields to be "Personal", meaning they will not overwrite an existing value on import. 

Working version currently exists, but full PR coming soon to CrowdAnki!

### Extensibility and Open Source
Free for all to use, modify, or sell this product.

Further source types are relatively easy to add due to the flexible nature of the backend
Instead of creating a Csv <-> CrowdAnki converter Brain Brew first goes through a middle layer called "Deck Parts". 
These consist of Notes, Headers, Note Models, and Media files. 

Each new source type to be added to Brain Brew (such as Markdown) need only be able to convert from Deck Parts <-> itself, and suddenly it can convert to and from all existing source types!

### Smart Csvs

Csvs only update the rows which have changed. 
Meaning a user can import *a subset* of their cards which have changed and still update the source file without deleting the cards they did not include.

##### Csv Splitting / Derivatives

Split data into multiple csvs so that your data is neatly organised however you like. 

The two following csv files contain information about England, but split into different csv files:

###### data-main.csv

| guid | country | flag | map | tags |
| ---- | ---- | ---- | ---- | ---- |
| "e+/O]%*qfk | England | <img src=""ug-flag-england.svg"" /> | <img src=""ug-map-england.png"" /> | UG::Europe |

###### data-capital.csv
| country | capital | capital de | capital es | capital fr | capital nb | 
| ---- | ---- | ---- | ---- | ---- | ---- |
| England | London | London | Londres | Londres | London | 

Brain Brew can be told that `data-capital` is a derivative of `data-main` in the build config file as such:

```yaml
- file: src/data/data-main.csv               # <---- Main
  note_model: Ultimate Geography
  derivatives:
    - file: src/data/data-country.csv
    - file: src/data/data-country-info.csv
    - file: src/data/data-capital.csv        # <---- Capital
    - file: src/data/data-capital-info.csv
    - file: src/data/data-capital-hint.csv
    # note_model: different_note_model
    # derivatives:
    # - file: derivative-of-a-derivative.csv
      # derivatives:
      # - file: infinite-nesting.csv
    - file: src/data/data-flag-similarity.csv
```

When run Brain Brew will perform the following steps for each derivative:
1. Finds which columns in the derivative csv match the main (only `country` in this case)
1. Go through each row in the derivative and find the row with matching values in the main file
1. Add in the extra columns (`capital` in each language) to that matching row in the main file 

###### Resulting csv data
| guid | country | flag | map | tags | capital | capital de | capital es | capital fr | capital nb | 
| ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- |
| "e+/O]%*qfk | England | <img src=""ug-flag-england.svg"" /> | <img src=""ug-map-england.png"" /> | UG::Europe | London | London | Londres | Londres | London | 

##### Note:

1. **Derivatives can also have derivatives**.

1. **Csv splitting works in both directions**, to and from csv.

1. **Derivatives can be given a Note Model**, which overrides their parent's note model for all the matched rows.

See the [Brain Brew Starter Project][BrainBrewStarter] for an example of Csv Derivatives working.



[BrainBrewStarter]: https://github.com/ohare93/brain-brew-starter