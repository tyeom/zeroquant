import { onMount, onCleanup, createEffect, on } from 'solid-js'
import { createChart, ColorType, HistogramSeries } from 'lightweight-charts'
import type { IChartApi, ISeriesApi, HistogramData, LogicalRange } from 'lightweight-charts'
import type { ChartSyncState } from './EquityCurve'

export interface DrawdownDataPoint {
  time: string | number
  value: number
}

// 데이터를 시간순으로 정렬하는 헬퍼 함수
function sortByTime(data: DrawdownDataPoint[]): DrawdownDataPoint[] {
  return [...data].sort((a, b) => {
    const timeA = typeof a.time === 'string' ? a.time : a.time.toString()
    const timeB = typeof b.time === 'string' ? b.time : b.time.toString()
    return timeA.localeCompare(timeB)
  })
}

interface DrawdownChartProps {
  data: DrawdownDataPoint[]
  height?: number
  maxDrawdownLine?: number
  colors?: {
    background?: string
    text?: string
    grid?: string
    drawdownColor?: string
    maxDrawdownColor?: string
  }
  // 차트 동기화 props
  chartId?: string
  syncState?: () => ChartSyncState | null
  onVisibleRangeChange?: (state: ChartSyncState) => void
}

export function DrawdownChart(props: DrawdownChartProps) {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let drawdownSeries: ISeriesApi<'Histogram'> | undefined
  let isExternalUpdate = false // 외부 업데이트 중 플래그 (무한 루프 방지)
  const chartId = props.chartId || 'drawdown-' + Math.random().toString(36).substr(2, 9)

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
    drawdownColor: '#ef4444',
    maxDrawdownColor: '#991b1b',
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
      height: props.height || 200,
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

    // Drawdown histogram (inverted - shows negative values) - lightweight-charts v5 API
    drawdownSeries = chart.addSeries(HistogramSeries, {
      color: colors.drawdownColor,
    })

    // Set initial data (시간순 정렬 필수)
    if (props.data && props.data.length > 0) {
      const sortedData = sortByTime(props.data)
      drawdownSeries.setData(sortedData as HistogramData[])
      chart.timeScale().fitContent()
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

  // Update data when props change (시간순 정렬 필수)
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (drawdownSeries && data && data.length > 0) {
          const sortedData = sortByTime(data)
          drawdownSeries.setData(sortedData as HistogramData[])
          chart?.timeScale().fitContent()
        }
      }
    )
  )

  return (
    <div
      ref={containerRef}
      class="w-full rounded-lg overflow-hidden"
      style={{ height: `${props.height || 200}px` }}
    />
  )
}
