import { onMount, onCleanup, createEffect, on } from 'solid-js'
import { createChart, ColorType, AreaSeries, LineSeries } from 'lightweight-charts'
import type { IChartApi, ISeriesApi, LineData, LogicalRange } from 'lightweight-charts'

export interface EquityDataPoint {
  time: string | number
  value: number
}

// 차트 동기화를 위한 공유 타입
export interface ChartSyncState {
  range: LogicalRange | null
  sourceId: string | null
}

// 데이터를 시간순으로 정렬하는 헬퍼 함수
function sortByTime(data: EquityDataPoint[]): EquityDataPoint[] {
  return [...data].sort((a, b) => {
    const timeA = typeof a.time === 'string' ? a.time : a.time.toString()
    const timeB = typeof b.time === 'string' ? b.time : b.time.toString()
    return timeA.localeCompare(timeB)
  })
}

interface EquityCurveProps {
  data: EquityDataPoint[]
  height?: number
  showBenchmark?: boolean
  benchmarkData?: EquityDataPoint[]
  colors?: {
    background?: string
    text?: string
    grid?: string
    equityColor?: string
    benchmarkColor?: string
    positiveArea?: string
    negativeArea?: string
  }
  // 차트 동기화 props
  chartId?: string
  syncState?: () => ChartSyncState | null
  onVisibleRangeChange?: (state: ChartSyncState) => void
}

export function EquityCurve(props: EquityCurveProps) {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let equitySeries: ISeriesApi<'Area'> | undefined
  let benchmarkSeries: ISeriesApi<'Line'> | undefined
  let isExternalUpdate = false // 외부 업데이트 중 플래그 (무한 루프 방지)
  const chartId = props.chartId || 'equity-' + Math.random().toString(36).substr(2, 9)

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
    equityColor: '#3b82f6',
    benchmarkColor: '#9ca3af',
    positiveArea: 'rgba(59, 130, 246, 0.2)',
    negativeArea: 'rgba(239, 68, 68, 0.2)',
  }

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  })

  onMount(() => {
    if (!containerRef) return

    const colors = getColors()

    chart = createChart(containerRef, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: containerRef.clientWidth,
      height: props.height || 300,
      crosshair: {
        mode: 1,
      },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: {
          top: 0.1,
          bottom: 0.1,
        },
      },
    })

    // Equity curve (area chart) - lightweight-charts v5 API
    equitySeries = chart.addSeries(AreaSeries, {
      lineColor: colors.equityColor,
      topColor: colors.positiveArea,
      bottomColor: 'transparent',
      lineWidth: 2,
    })

    // Benchmark line (optional)
    if (props.showBenchmark) {
      benchmarkSeries = chart.addSeries(LineSeries, {
        color: colors.benchmarkColor,
        lineWidth: 1,
        lineStyle: 2, // Dashed
      })
    }

    // Set initial data (시간순 정렬 필수)
    if (props.data && props.data.length > 0) {
      const sortedData = sortByTime(props.data)
      equitySeries.setData(sortedData as LineData[])
      chart.timeScale().fitContent()
    }

    if (props.showBenchmark && props.benchmarkData && benchmarkSeries) {
      const sortedBenchmark = sortByTime(props.benchmarkData)
      benchmarkSeries.setData(sortedBenchmark as LineData[])
    }

    // 차트 동기화: 범위 변경 감지
    if (props.onVisibleRangeChange) {
      chart.timeScale().subscribeVisibleLogicalRangeChange((logicalRange) => {
        if (isExternalUpdate || !logicalRange) return
        props.onVisibleRangeChange?.({
          range: logicalRange,
          sourceId: chartId,
        })
      })
    }

    // Handle resize
    const handleResize = () => {
      if (chart && containerRef) {
        chart.applyOptions({ width: containerRef.clientWidth })
      }
    }

    window.addEventListener('resize', handleResize)

    onCleanup(() => {
      window.removeEventListener('resize', handleResize)
      if (chart) {
        chart.remove()
      }
    })
  })

  // 차트 동기화: 다른 차트에서 범위가 변경되면 이 차트도 업데이트
  createEffect(
    on(
      () => props.syncState?.(),
      (syncState) => {
        if (!chart || !syncState?.range) return
        // 자기 자신이 소스인 경우 무시
        if (syncState.sourceId === chartId) return

        isExternalUpdate = true
        chart.timeScale().setVisibleLogicalRange(syncState.range)
        // 다음 틱에 플래그 해제
        setTimeout(() => { isExternalUpdate = false }, 0)
      }
    )
  )

  // Update equity data (시간순 정렬 필수)
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (equitySeries && data && data.length > 0) {
          const sortedData = sortByTime(data)
          equitySeries.setData(sortedData as LineData[])
          chart?.timeScale().fitContent()
        }
      }
    )
  )

  // Update benchmark data (시간순 정렬 필수)
  createEffect(
    on(
      () => props.benchmarkData,
      (data) => {
        if (benchmarkSeries && data && data.length > 0) {
          const sortedData = sortByTime(data)
          benchmarkSeries.setData(sortedData as LineData[])
        }
      }
    )
  )

  return (
    <div
      ref={containerRef}
      class="w-full rounded-lg overflow-hidden"
      style={{ height: `${props.height || 300}px` }}
    />
  )
}
