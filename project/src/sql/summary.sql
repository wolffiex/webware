SELECT json_build_object(
    'time', time,
    'outdoor_temp', outdoor_temp,
    'humidity', humidity,
    'uvi', uvi,
    'rain_rate', rain_rate
) FROM weather ORDER BY time DESC LIMIT 1;

WITH PressureBuckets AS (
    SELECT
        time_bucket('10 minutes', time) as bucket,
        AVG(pressure) as avg_pressure
    FROM weather
    WHERE time >= NOW() - INTERVAL '2 hours'
    GROUP BY bucket
    ORDER BY bucket DESC
)
SELECT json_build_object(
    'pressure_difference', (SELECT avg_pressure FROM PressureBuckets ORDER BY bucket DESC LIMIT 1) - 
        (SELECT avg_pressure FROM PressureBuckets ORDER BY bucket DESC OFFSET 6 LIMIT 1)
) FROM PressureBuckets
LIMIT 1;

WITH time_boundaries AS (
    SELECT
        DATE_TRUNC('day', NOW() - INTERVAL '1 day') AT TIME ZONE 'UTC' AT TIME ZONE 'PST' as start_time,
        DATE_TRUNC('day', NOW()) AT TIME ZONE 'UTC' AT TIME ZONE 'PST' as end_time
),
relevant_rows AS (
    SELECT time, outdoor_temp, uvi FROM weather, time_boundaries
    WHERE time >= time_boundaries.start_time
    AND time < time_boundaries.end_time
)
SELECT 
    row_to_json(t) 
FROM 
  (
  SELECT 
    json_build_object(
      'time', max_temp.time, 
      'degrees', max_temp.outdoor_temp
    ) AS high_temp,
    json_build_object(
      'time', min_temp.time, 
      'degrees', min_temp.outdoor_temp
    ) AS low_temp,
    json_build_object(
      'time', max_uvi.time, 
      'value', max_uvi.uvi
    ) AS high_uvi
  FROM 
      (SELECT time, outdoor_temp FROM relevant_rows
       ORDER BY outdoor_temp DESC LIMIT 1) AS max_temp,

      (SELECT time, outdoor_temp FROM relevant_rows
       ORDER BY outdoor_temp ASC LIMIT 1) AS min_temp,

      (SELECT time, uvi FROM relevant_rows 
       ORDER BY uvi DESC LIMIT 1) AS max_uvi
  ) t;
