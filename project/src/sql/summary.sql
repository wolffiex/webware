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
    ) AS uvi
  FROM 
      (SELECT time, outdoor_temp FROM weather 
       WHERE time >= DATE_TRUNC('day', NOW() - INTERVAL '1 day') 
       AND time < DATE_TRUNC('day', NOW()) 
       ORDER BY outdoor_temp DESC LIMIT 1) AS max_temp,

      (SELECT time, outdoor_temp FROM weather
       WHERE time >= DATE_TRUNC('day', NOW() - INTERVAL '1 day') 
       AND time < DATE_TRUNC('day', NOW()) 
       ORDER BY outdoor_temp ASC LIMIT 1) AS min_temp,

      (SELECT time, uvi FROM weather
       WHERE time >= DATE_TRUNC('day', NOW() - INTERVAL '1 day') 
       AND time < DATE_TRUNC('day', NOW()) 
       ORDER BY uvi DESC LIMIT 1) AS max_uvi
  ) t;
