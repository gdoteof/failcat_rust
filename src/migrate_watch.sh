while wrangler d1 execute failcat --command "SELECT 'cars' AS table_name, COUNT(*) AS row_count FROM cars UNION ALL SELECT 'temp_table' AS table_name, COUNT(*) AS row_count FROM temp_table;";
do
	sleep 10
done
