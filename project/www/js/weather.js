console.log("hi from weaher");

export default async function ({ source, node }) {
  console.log("wheeat got called", source);
  let pressureData = [];
  let temperatureData = [];
  for await (const data of source) {
    pressureData.push({ x: data.time_bucket, y: data.pressure });
    temperatureData.push({ x: data.time_bucket, y: data.outdoor_temp });
  }
  new window.Chart(node, getTwoDayChart(pressureData, temperatureData));
}

function getTwoDayChart(pressureData, temperatureData) {
  return {
    type: "line",
    data: {
      datasets: [
        {
          label: "Pressure",
          data: pressureData,
          fill: false,
          borderColor: "rgb(75, 192, 192)",
          tension: 0.1,
          yAxisID: "pressure",
        },
        {
          label: "Temperature",
          data: temperatureData,
          fill: false,
          borderColor: "rgb(255, 99, 132)",
          tension: 0.1,
          yAxisID: "temperature",
        },
      ],
    },
    options: {
      scales: {
        x: {
          type: "time",
          time: {
            unit: "minute",
            stepSize: 60,
          },
          title: {
            display: true,
            text: "Time",
          },
        },
        temperature: {
          type: "linear",
          position: "left", // Temperature on the left
          beginAtZero: false,
          title: {
            display: true,
            text: "Temperature (°C)",
          },
          grid: {
            drawOnChartArea: true, // Temperature has grid lines
          },
        },
        pressure: {
          type: "linear",
          position: "right", // Pressure on the right
          beginAtZero: false,
          title: {
            display: true,
            text: "Pressure (hPa)",
          },
          grid: {
            drawOnChartArea: false, // Pressure does not have grid lines
          },
        },
      },
    },
  };
}
