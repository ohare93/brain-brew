# class TestConstructor:
#     def test_runs(self):
#         with patch.object(CsvsGenerate, "__init__", return_value=None) as mock_csv_tr, \
#                 patch.object(DeckPartHolder, "from_part_pool", return_value=Mock()), \
#                 patch.object(CsvFile, "create_or_get", return_value=Mock()):
#
#             data = YamlObject.read_to_dict(TestFiles.BuildConfig.ONE_OF_EACH_TYPE)
#             builder = TopLevelRecipeBuilder.from_list(data)
#             builder.execute()
#
#             assert len(builder.tasks) == 1
#             assert mock_csv_tr.call_count == 1
